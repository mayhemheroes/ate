/*
 * To change this license header, choose License Headers in Project Properties.
 * To change this template file, choose Tools | Templates
 * and open the template in the editor.
 */
package com.tokera.ate.dto.msg;

import com.fasterxml.jackson.annotation.JsonIgnore;
import com.fasterxml.jackson.annotation.JsonProperty;
import com.tokera.ate.annotations.YamlTag;
import com.tokera.ate.common.CopyOnWrite;

import javax.enterprise.context.Dependent;
import javax.validation.constraints.NotNull;
import java.io.Serializable;

/**
 * Represents metadata about a data message that was placed on the distributed commit log
 */
@Dependent
@YamlTag("msg.meta")
public class MessageMetaDto implements Serializable {

    private static final long serialVersionUID = -1978186226449951313L;

    @JsonProperty
    @NotNull
    private String key;
    @JsonProperty
    @NotNull
    private long partition;
    @JsonProperty
    @NotNull
    private long offset;

    @JsonIgnore
    private transient boolean _immutable = false;

    @SuppressWarnings("initialization.fields.uninitialized")
    @Deprecated
    public MessageMetaDto() {
    }

    public MessageMetaDto(String key, long partition, long offset) {
        this.key = key;
        this.partition = partition;
        this.offset = offset;
    }
    
    public long getPartition() {
        return partition;
    }

    public void setPartition(long partition) {
        assert this._immutable == false;
        this.partition = partition;
    }

    public long getOffset() {
        return offset;
    }

    public void setOffset(long offset){
        assert this._immutable == false;
        this.offset = offset;
    }

    public void immutalize() {
        if (this instanceof CopyOnWrite) {
            ((CopyOnWrite)this).copyOnWrite();
        }
        this._immutable = true;
    }

    public String getKey() {
        return key;
    }

    public void setKey(String key) {
        this.key = key;
    }
}
