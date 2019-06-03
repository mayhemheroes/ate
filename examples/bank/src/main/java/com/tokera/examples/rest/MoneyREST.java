package com.tokera.examples.rest;

import com.google.common.collect.Lists;
import com.tokera.ate.common.LoggerHook;
import com.tokera.ate.delegates.AteDelegate;
import com.tokera.ate.dto.msg.MessagePrivateKeyDto;
import com.tokera.ate.dto.msg.MessagePublicKeyDto;
import com.tokera.examples.dao.*;
import com.tokera.examples.dto.*;

import javax.enterprise.context.ApplicationScoped;
import javax.inject.Inject;
import javax.ws.rs.*;
import javax.ws.rs.core.MediaType;
import javax.ws.rs.core.Response;

@ApplicationScoped
@Path("/money")
public class MoneyREST {
    protected AteDelegate d = AteDelegate.get();

    @SuppressWarnings("initialization.fields.uninitialized")
    @Inject
    private LoggerHook LOG;

    @POST
    @Path("/print")
    @Produces({"text/yaml", MediaType.APPLICATION_JSON})
    @Consumes({"text/yaml", MediaType.APPLICATION_JSON})
    public TransactionToken printMoney(CreateAssetRequest request) {
        MessagePublicKeyDto coiningKey = d.implicitSecurity.enquireDomainKey(request.type, true);

        Asset asset = new Asset(request.type, request.value);
        d.authorization.authorizeEntityPublicRead(asset);
        d.authorization.authorizeEntityWrite(coiningKey, asset);
        d.headIO.mergeLater(asset);

        AssetShare assetShare = new AssetShare(asset, request.value);
        d.authorization.authorizeEntityWrite(request.ownershipKey, assetShare);
        asset.shares.add(assetShare.id);

        d.headIO.mergeLater(asset);
        d.headIO.mergeLater(assetShare);

        //LOG.info(d.yaml.serializeObj(asset));
        //LOG.info(d.yaml.serializeObj(assetShare));

        d.headIO.merge(asset.addressableId().partition(), coiningKey);
        d.headIO.merge(asset.addressableId().partition(), request.ownershipKey);

        return new TransactionToken(Lists.newArrayList(new ShareToken(assetShare, request.ownershipKey)));
    }

    @POST
    @Path("/burn")
    @Produces(MediaType.TEXT_PLAIN)
    @Consumes({"text/yaml", MediaType.APPLICATION_JSON})
    public boolean burnMoney(RedeemAssetRequest request) {
        AssetShare assetShare = d.headIO.get(request.shareToken.getShare(), AssetShare.class);
        if (d.daoHelper.hasImplicitAuthority(assetShare, request.validateType) == false) {
            throw new WebApplicationException("Asset is not of the correct type.", Response.Status.NOT_ACCEPTABLE);
        }
        assetShare.trustInheritWrite = false;
        assetShare.trustAllowWrite.clear();
        d.headIO.merge(assetShare);
        return true;
    }
}