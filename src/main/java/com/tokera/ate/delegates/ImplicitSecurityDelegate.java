package com.tokera.ate.delegates;

import com.google.common.collect.Lists;
import com.tokera.ate.common.LoggerHook;
import com.tokera.ate.common.MapTools;
import com.tokera.ate.dto.msg.MessagePrivateKeyDto;
import com.tokera.ate.dto.msg.MessagePublicKeyDto;
import com.tokera.ate.events.RegisterPublicTopicEvent;
import com.tokera.ate.io.api.IPartitionKey;
import com.tokera.ate.providers.PartitionKeySerializer;
import com.tokera.ate.scopes.Startup;
import com.tokera.ate.units.Alias;
import com.tokera.ate.units.DomainName;
import com.tokera.ate.units.PlainText;
import org.apache.commons.codec.binary.Base64;
import org.apache.commons.io.IOUtils;
import org.checkerframework.checker.nullness.qual.Nullable;
import org.junit.jupiter.api.Assertions;
import org.reflections.Reflections;
import org.reflections.scanners.ResourcesScanner;
import org.reflections.util.ClasspathHelper;
import org.reflections.util.ConfigurationBuilder;
import org.reflections.util.FilterBuilder;
import org.xbill.DNS.*;

import javax.annotation.PostConstruct;
import javax.enterprise.context.ApplicationScoped;
import javax.enterprise.event.Observes;
import javax.inject.Inject;
import javax.naming.NamingException;
import javax.ws.rs.WebApplicationException;
import javax.ws.rs.core.Response;
import java.io.IOException;
import java.io.InputStream;
import java.net.InetAddress;
import java.net.UnknownHostException;
import java.util.*;
import java.util.concurrent.ConcurrentHashMap;
import java.util.function.Function;

import static org.reflections.util.Utils.findLogger;

/**
 * Uses properties of the Internet to derive authentication and authorization rules
 */
@Startup
@ApplicationScoped
public class ImplicitSecurityDelegate {

    private AteDelegate d = AteDelegate.get();
    @SuppressWarnings("initialization.fields.uninitialized")
    @Inject
    private LoggerHook LOG;
    
    private static final Cache g_dnsCache = new Cache();

    private ConcurrentHashMap<String, String> enquireTxtOverride = new ConcurrentHashMap<>();
    private ConcurrentHashMap<String, List<String>> enquireAddressOverride = new ConcurrentHashMap<>();

    private Reflections resReflection;
    private Map<String, MessagePublicKeyDto> embeddedKeys = new HashMap<>();

    @SuppressWarnings("initialization.fields.uninitialized")
    private SimpleResolver m_resolver;
    
    static {
        g_dnsCache.setMaxNCache(300);
        g_dnsCache.setMaxCache(300);
        g_dnsCache.setMaxEntries(20000);
    }

    public ImplicitSecurityDelegate() {
        Reflections.log = null;
        resReflection = new Reflections(
                new ConfigurationBuilder()
                        .filterInputsBy(new FilterBuilder()
                                .exclude("(.*)\\.so$")
                                .include("embedded-keys\\.(.*)"))
                        .setUrls(ClasspathHelper.forClassLoader())
                        .setScanners(new ResourcesScanner()));
        Reflections.log = findLogger(Reflections.class);
    }
    
    @PostConstruct
    public void init() {
        try {
            m_resolver = new SimpleResolver();
            m_resolver.setTCP(true);
            m_resolver.setTimeout(4);
            //m_resolver.setTSIGKey(null);
            m_resolver.setAddress(InetAddress.getByName(d.bootstrapConfig.getDnsServer()));

            List<MessagePublicKeyDto> keys = this.loadEmbeddedKeys();
            for (MessagePublicKeyDto key : keys) {
                this.embeddedKeys.put(key.getPublicKeyHash(), key);
            }
        } catch (UnknownHostException ex) {
            LOG.error(ex);
        }
    }

    public @Nullable MessagePublicKeyDto enquireDomainKey(@DomainName String domain, boolean shouldThrow)
    {
        return enquireDomainKey(domain, shouldThrow, null);
    }

    public @Nullable MessagePublicKeyDto enquireDomainKey(@DomainName String domain, boolean shouldThrow, IPartitionKey partitionKey)
    {
        return enquireDomainKey(d.bootstrapConfig.getImplicitAuthorityAlias(), domain, shouldThrow, partitionKey);
    }

    public @Nullable MessagePublicKeyDto enquireDomainKey(@DomainName String domain, boolean shouldThrow, IPartitionKey partitionKey, Function<String, MessagePublicKeyDto> publicKeyResolver) {
        return enquireDomainKey(d.bootstrapConfig.getImplicitAuthorityAlias(), domain, shouldThrow, null, partitionKey, publicKeyResolver);
    }

    public @Nullable MessagePublicKeyDto enquireDomainKey(String prefix, @DomainName String domain, boolean shouldThrow)
    {
        return enquireDomainKey(prefix, domain, shouldThrow, domain, null, null);
    }

    public @Nullable MessagePublicKeyDto enquireDomainKey(String prefix, @DomainName String domain, boolean shouldThrow, @Nullable IPartitionKey partitionKey)
    {
        return enquireDomainKey(prefix, domain, shouldThrow, domain, partitionKey, null);
    }

    public @Nullable MessagePublicKeyDto enquireDomainKey(String prefix, @DomainName String domain, boolean shouldThrow, @Alias String alias)
    {
        return enquireDomainKey(prefix, domain, shouldThrow, alias, null, null);
    }

    public @Nullable MessagePublicKeyDto enquireDomainKey(String prefix, @DomainName String domain, boolean shouldThrow, @Nullable @Alias String alias, @Nullable IPartitionKey partitionKey, @Nullable Function<String, MessagePublicKeyDto> publicKeyResolver)
    {
        String publicKeyHash = enquireDomainString(prefix + "." + domain, shouldThrow);
        if (publicKeyHash == null) {
            if (shouldThrow) {
                throw new RuntimeException("No implicit authority found at domain name [" + prefix + "." + domain + "] (missing TXT record).");
            }
            return null;
        }

        MessagePublicKeyDto ret = null;
        if (publicKeyResolver != null) {
            ret = publicKeyResolver.apply(publicKeyHash);
        } else {
            if (partitionKey != null) {
                ret = d.io.publicKeyOrNull(partitionKey, publicKeyHash);
            } else {
                ret = d.io.publicKeyOrNull(publicKeyHash);
            }
            if (ret == null) {
                ret = d.currentRights.getRightsWrite()
                        .stream()
                        .filter(k -> publicKeyHash.equals(k.getPublicKeyHash()))
                        .findFirst()
                        .orElse(null);
            }
        }

        if (ret == null) {
            if (shouldThrow) {
                throw new RuntimeException("Unknown implicit authority found at domain name [" + prefix + "." + domain + "] (public key is missing with hash [" + publicKeyHash + "]).");
            }
        } else {
            ret = new MessagePublicKeyDto(ret);
            if (alias != null) {
                ret.setAlias(alias);
            }
        }

        return ret;
    }

    public String generateDnsTxtRecord(MessagePublicKeyDto key) {
        return generateDnsTxtRecord(key, d.requestContext.getPartitionKeyScopeOrNull());
    }

    public String generateDnsTxtRecord(MessagePublicKeyDto key, IPartitionKey partitionKey) {
        if (partitionKey == null) {
            String ret = key.getPublicKeyHash();
            if (ret == null) throw new RuntimeException("Failed to generate the DNS TXT record entry as the hash of the public key could not be generated.");
        }
        if (d.io.publicKeyOrNull(partitionKey, key.getPublicKeyHash()) == null) {
            d.io.merge(partitionKey, key);
        }
        String partitionKeyTxt = new PartitionKeySerializer().write(partitionKey);
        assert partitionKeyTxt != null : "@AssumeAssertion(nullness): Must not be null";
        return Base64.encodeBase64URLSafeString(partitionKeyTxt.getBytes()) + ":" + key.getPublicKeyHash();
    }

    public List<String> enquireDomainAddresses(@DomainName String domain, boolean shouldThrow) {
        if (domain.contains(":")) {
            String[] comps = domain.split(":");
            if (comps.length >= 1) domain = comps[0];
        }
        if (domain.endsWith(".") == false) domain += ".";

        List<String> override = MapTools.getOrNull(enquireAddressOverride, domain);
        if (override != null) {
            return override;
        }

        if ("127.0.0.1.".equals(domain)) {
            return Collections.singletonList("127.0.0.1");
        }
        if ("localhost.".equals(domain)) {
            return Collections.singletonList("localhost");
        }

        try {

            Lookup lookup = new Lookup(domain, Type.ANY, DClass.IN);
            lookup.setResolver(m_resolver);
            lookup.setCache(g_dnsCache);

            final Record[] records = lookup.run();
            if (lookup.getResult() != Lookup.SUCCESSFUL) {
                if (shouldThrow && lookup.getResult() == Lookup.UNRECOVERABLE) {
                    throw new WebApplicationException("Failed to lookup DNS record on [" + domain + "] - " + lookup.getErrorString());
                }
                this.LOG.debug("dns(" + domain + ")::" + lookup.getErrorString());
                return new ArrayList<>();
            }

            List<String> ret = new ArrayList<>();
            for (Record record : records) {
                //this.LOG.info("dns(" + domain + ")::record(" + record.toString() + ")");

                if (record instanceof ARecord) {
                    ARecord a = (ARecord) record;
                    ret.add(a.getAddress().getHostAddress().trim().toLowerCase());
                }
                if (record instanceof AAAARecord) {
                    AAAARecord aaaa = (AAAARecord) record;
                    ret.add("[" + aaaa.getAddress().getHostAddress().trim().toLowerCase() + "]");
                }
            }
            Collections.sort(ret);
            return ret;
        } catch (TextParseException ex) {
            if (shouldThrow) {
                throw new WebApplicationException(ex);
            }
            this.LOG.info("dns(" + domain + ")::" + ex.getMessage());
            return new ArrayList<>();
        }
    }
    
    public @Nullable @PlainText String enquireDomainString(@DomainName String domain, boolean shouldThrow)
    {
        String override = MapTools.getOrNull(enquireTxtOverride, domain);
        if (override != null) {
            return override;
        }

        try
        {
            @DomainName String implicitAuth = domain;
            if (implicitAuth.endsWith(".") == false) implicitAuth += ".";
            
            Lookup lookup = new Lookup(implicitAuth, Type.ANY, DClass.IN);
            lookup.setResolver(m_resolver);
            lookup.setCache(g_dnsCache);

            final Record[] records = lookup.run();
            if (lookup.getResult() != Lookup.SUCCESSFUL) {
                if (shouldThrow && lookup.getResult() == Lookup.UNRECOVERABLE) {
                    throw new WebApplicationException("Failed to lookup DNS record on [" + domain + "] - " + lookup.getErrorString());
                }
                this.LOG.debug("dns(" + domain + ")::" + lookup.getErrorString());
                return null;
            }
            
            for (Record record : records) {
                //this.LOG.info("dns(" + domain + ")::record(" + record.toString() + ")");
                
                if (record instanceof TXTRecord) {
                    TXTRecord txt = (TXTRecord)record;
                    
                    final List strings = txt.getStrings();
                    if (strings.isEmpty()) {
                        continue;
                    }

                    StringBuilder sb = new StringBuilder();
                    for (Object str : strings) {
                        if (str == null) continue;
                        sb.append(str.toString());
                    }
                    if (sb.length() <= 0) continue;
                    return sb.toString();
                }
            }

            //this.LOG.info("dns(" + domain + ")::no_record");
            return null;
        } catch (TextParseException ex) {
            if (shouldThrow) {
                throw new WebApplicationException(ex);
            }
            this.LOG.info("dns(" + domain + ")::" + ex.getMessage());
            return null;
        }
    }

    public ConcurrentHashMap<String, String> getEnquireTxtOverride() {
        return enquireTxtOverride;
    }

    public ConcurrentHashMap<String, List<String>> getEnquireAddressOverride() {
        return enquireAddressOverride;
    }

    public @Nullable MessagePublicKeyDto findEmbeddedKeyOrNull(String hash)
    {
        return MapTools.getOrNull(this.embeddedKeys, hash);
    }

    private List<MessagePublicKeyDto> loadEmbeddedKeys() {
        List<MessagePublicKeyDto> ret = new ArrayList<>();

        for (String file : resReflection.getResources(n -> true)) {
            try {
                if (file.startsWith("embedded-keys/") == false)
                    continue;

                InputStream inputStream = ClassLoader.getSystemResourceAsStream(file);
                assert inputStream != null : "@AssumeAssertion(nullness): Must not be null";
                Assertions.assertNotNull(inputStream);

                String keysFile = IOUtils.toString(inputStream, com.google.common.base.Charsets.UTF_8);

                for (String _keyTxt : keysFile.split("\\.\\.\\.")) {
                    String keyTxt = _keyTxt + "...";

                    Object obj = AteDelegate.get().yaml.deserializeObj(keyTxt);
                    if (obj instanceof MessagePublicKeyDto) {
                        MessagePublicKeyDto key = (MessagePublicKeyDto) obj;
                        ret.add(key);
                    }
                }

            } catch (IOException ex) {
                throw new WebApplicationException("Failed to load standard rate card", ex, Response.Status.INTERNAL_SERVER_ERROR);
            }
        }

        return ret;
    }
}