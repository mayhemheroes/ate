/*
 * To change this license header, choose License Headers in Project Properties.
 * To change this template file, choose Tools | Templates
 * and open the template in the editor.
 */
package com.tokera.ate.dao;

import com.fasterxml.jackson.annotation.JsonIgnore;
import com.tokera.ate.dto.msg.MessagePrivateKeyDto;
import com.tokera.ate.units.Alias;
import com.tokera.ate.units.DaoId;

import java.util.Map;
import java.util.Set;
import java.util.UUID;

/**
 * Interface that provides access rights to different roles through the Tokera
 * ecosystem. If a user is able to read this record then they are able to
 * gain access to the things that it has access to
 */
public interface IRights
{
    @JsonIgnore
    @DaoId UUID getId();

    @JsonIgnore
    Set<MessagePrivateKeyDto> getRightsRead();

    @JsonIgnore
    Set<MessagePrivateKeyDto> getRightsWrite();

    @JsonIgnore
    @Alias String getRightsAlias();

    default void onAddRight(IRoles to) {
    }

    default void onRemoveRight(IRoles from) {
    }

    default boolean readOnly() {
        return false;
    }
}