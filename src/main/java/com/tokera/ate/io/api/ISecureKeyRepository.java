package com.tokera.ate.io.api;

import com.tokera.ate.dto.msg.MessagePrivateKeyDto;
import com.tokera.ate.dto.msg.MessagePublicKeyDto;
import com.tokera.ate.units.Hash;
import com.tokera.ate.units.Secret;
import org.checkerframework.checker.nullness.qual.Nullable;

/**
 * Interface used to get and set the encryption keys under a particular metadata context
 * (This interface can be used to erase data for compliance or security reasons, e.g. GDPR)
 */
public interface ISecureKeyRepository {

    /**
     * Gets a secret key based on a public key and a hash of the secret key
     * @param partitionKey The partition that this secure key is related to
     * @param secretKeyHash Hash of the secret key we are attempting to retrieve
     * @param accessKey Access key that is used to retrieve this secret key
     * @return The secret key or null if it can not be found
     */
    @Nullable @Secret byte[] get(IPartitionKey partitionKey, @Hash String secretKeyHash, MessagePrivateKeyDto accessKey);

    /**
     * Adds a secret key into the repository
     * @param partitionKey The partition that this secure key is related to
     * @param secretKey The secret key to be added
     * @param publicKeyHash Hash of the public key related to the access key
     */
    void put(IPartitionKey partitionKey, @Secret byte[] secretKey, @Hash String publicKeyHash);

    /**
     * @return Returns true if the encryption key exists in this repository
     */
    boolean exists(IPartitionKey partitionKey, @Hash String secretKeyHash, @Hash String publicKeyHash);
}
