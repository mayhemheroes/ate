package com.tokera.ate.dao.base;

import com.fasterxml.jackson.annotation.JsonIgnore;
import com.tokera.ate.common.Immutalizable;
import com.tokera.ate.dao.PUUID;
import com.tokera.ate.delegates.AteDelegate;
import com.tokera.ate.io.api.IPartitionKey;
import com.tokera.ate.units.DaoId;
import org.checkerframework.checker.nullness.qual.Nullable;

import java.io.Serializable;
import java.util.Set;
import java.util.UUID;

/**
 * Represents the common fields and methods of all data objects that are stored in the ATE data-store
 */
public abstract class BaseDao implements Serializable, Immutalizable {

    transient @JsonIgnore @Nullable Set<UUID> _mergesVersions = null;
    transient @JsonIgnore @Nullable UUID _previousVersion = null;
    transient @JsonIgnore @Nullable UUID _version = null;
    transient @JsonIgnore boolean _immutable = false;
    transient @JsonIgnore @Nullable IPartitionKey _partitionKey = null;

    /**
     * @return Returns the unique primary key of this data entity within the
     * scope of the partition
     */
    public abstract @DaoId UUID getId();

    /**
     * @return Returns an identifier that can be used to reference this data
     * object even if its in a different partition
     */
    @JsonIgnore
    public PUUID addressableId() {
        return new PUUID(this.partitionKey(), this.getId());
    }

    /**
     * @return Returns the partition that this object belongs too based on its inheritance tree and the current
     * partition key resolver
     */
    @JsonIgnore
    public IPartitionKey partitionKey() {
        IPartitionKey ret = _partitionKey;
        if (ret != null) return ret;
        ret = AteDelegate.get().io.partitionResolver().resolve(this);
        return ret;
    }
    
    /**
     * @return Returns the parent object that this object is attached to
     */
    public abstract @JsonIgnore @Nullable @DaoId UUID getParentId();
    
    @Override
    public int hashCode() {
        int hash = 0;
        hash += getId().hashCode();
        return hash;
    }

    @Override
    public boolean equals(@Nullable Object object) {
        if (object == null) {
            return false;
        }
        if (!(object.getClass() == this.getClass())) {
            return false;
        }
        BaseDao other = (BaseDao) object;
        if (!this.getId().equals(other.getId())) {
            return false;
        }
        return true;
    }

    @Override
    public String toString() {
        return this.getClass().getSimpleName() + "[ id=" + getId().toString().substring(0, 8) + "... ]";
    }

    @Override
    public void immutalize() {
        this._immutable = true;
    }

    boolean hasSaved() {
        return this._version != null;
    }

    void assertStillMutable() {
        assert _immutable == false;
    }

    void newVersion() {
        UUID oldVerison = _version;
        _version = UUID.randomUUID();
        _previousVersion = oldVerison;
    }
}