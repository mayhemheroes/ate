package com.tokera.ate.client;

import com.tokera.ate.delegates.AteDelegate;
import com.tokera.ate.dto.msg.MessagePrivateKeyDto;
import com.tokera.ate.providers.*;
import com.tokera.ate.units.EmailAddress;
import com.tokera.ate.units.Port;
import com.tokera.ate.units.Secret;
import org.checkerframework.checker.nullness.qual.Nullable;
import org.jboss.resteasy.client.jaxrs.ResteasyClient;
import org.jboss.resteasy.client.jaxrs.ResteasyClientBuilder;
import org.jboss.resteasy.client.jaxrs.ResteasyWebTarget;
import org.jboss.resteasy.plugins.providers.jackson.ResteasyJackson2Provider;

import javax.ws.rs.WebApplicationException;
import javax.ws.rs.client.Entity;
import javax.ws.rs.core.MultivaluedMap;
import javax.ws.rs.core.Response;
import java.security.InvalidParameterException;
import java.util.Map;

public class RawClientBuilder {

    private String server = "127.0.0.1";
    private String prefixForFs = "/fs";
    private String prefixForRest = "/api";
    private boolean secure = false;
    private @Nullable @Port Integer port = null;
    private @Nullable String session;
    private @Nullable String loginViaRestPostPath;
    private @Nullable Entity<?> loginViaRestPostEntity;
    private @Nullable MultivaluedMap<String, Object> loginViaRestPostQueryParams;

    public RawClientBuilder server(String server) {
        this.server = server;
        return this;
    }

    public RawClientBuilder port(int port) {
        this.port = port;
        return this;
    }

    public RawClientBuilder secure(boolean val) {
        this.secure = val;
        return this;
    }

    public RawClientBuilder prefixForRest(String prefix) {
        this.prefixForRest = prefix;
        return this;
    }

    public RawClientBuilder prefixForFs(String prefix) {
        this.prefixForFs = prefix;
        return this;
    }

    public RawClientBuilder withSession(@Nullable String session) {
        if (session == null) {
            throw new WebApplicationException("The session can not be empty.");
        }

        this.session = session;
        this.loginViaRestPostPath = null;
        this.loginViaRestPostEntity = null;
        this.loginViaRestPostQueryParams = null;
        return this;
    }

    public RawClientBuilder withLoginPost(String path, Entity<?> entity) {
        return withLoginPost(path, entity, null);
    }

    public RawClientBuilder withLoginPost(String path, Entity<?> entity, MultivaluedMap<String, Object> queryParams) {
        this.session = null;
        this.loginViaRestPostPath = path;
        this.loginViaRestPostEntity = entity;
        this.loginViaRestPostQueryParams = queryParams;
        return this;
    }

    public RawClientBuilder withLoginPassword(@EmailAddress String username, @Secret String password, @Secret String code) {
        String path = "/login/byUsername/" + username + "/login?expiresMins=10&code=" + code;
        return withLoginPost(path, Entity.text(password));
    }

    public RawClientBuilder withLoginKey(String username, MessagePrivateKeyDto key) {
        return withLoginPost("/login/byKey/rootLogin", Entity.json(key));
    }

    public RawClientBuilder withLoginToken(String urlBaseAndPrefix, String token) {
        return withLoginPost("/login/token", Entity.text(token));
    }

    public static String generateServerUrl(boolean secure, String server, @Nullable Integer port) {
        StringBuilder sb = new StringBuilder();
        if (secure) {
            sb.append("https://");
        } else {
            sb.append("http://");
        }
        sb.append(server);

        if (port != null) {
            sb.append(":").append(port);
        } else if (secure == false) {
            sb.append(":8080");
        }
        return sb.toString();
    }

    public RawClient build() {
        String urlBase = generateServerUrl(this.secure, this.server, this.port);

        if (this.loginViaRestPostPath != null) {
            Entity<?> loginViaRestPostEntity = this.loginViaRestPostEntity;
            if (loginViaRestPostEntity == null) {
                throw new InvalidParameterException("You must specify a login entity data to be posted to the URL.");
            }

            String url = urlBase + prefixForRest + loginViaRestPostPath;

            AteDelegate d = AteDelegate.get();
            Response response = TestTools.restPost(null, url, loginViaRestPostEntity, loginViaRestPostQueryParams, null);

            String auth = response.getHeaderString("Authorization");
            d.genericLogger.info("auth:\n" + auth);

            this.session = auth;
        }

        return new RawClient(urlBase, this.session, this.prefixForRest, this.prefixForFs);
    }

    private static ResteasyWebTarget target(String urlBaseAndPrefix, String postfix) {
        ResteasyClient client = new ResteasyClientBuilder()
                .register(new YamlProvider())
                .register(new UuidSerializer())
                .register(new PartitionKeySerializer())
                .register(new PuuidSerializer())
                .register(new CountLongSerializer())
                .build();

        return client.target(urlBaseAndPrefix + postfix);
    }

    public String getSession() {
        return this.session;
    }
}
