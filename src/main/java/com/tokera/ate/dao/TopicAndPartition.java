package com.tokera.ate.dao;

import com.fasterxml.jackson.annotation.JsonIgnore;
import com.fasterxml.jackson.annotation.JsonTypeName;
import com.fasterxml.jackson.databind.annotation.JsonDeserialize;
import com.fasterxml.jackson.databind.annotation.JsonSerialize;
import com.tokera.ate.annotations.YamlTag;
import com.tokera.ate.enumerations.DataPartitionType;
import com.tokera.ate.io.api.IPartitionKey;
import com.tokera.ate.providers.GenericPartitionKeyJsonDeserializer;
import com.tokera.ate.providers.GenericPartitionKeyJsonSerializer;
import com.tokera.ate.providers.PartitionKeySerializer;
import org.apache.kafka.common.TopicPartition;

import javax.enterprise.context.Dependent;
import java.io.Serializable;

@Dependent
@YamlTag("topicpart")
@JsonTypeName("topicpart")
public final class TopicAndPartition implements Serializable {
    private static final long serialVersionUID = -4780665965525636535L;

    private String topic;
    private int partition;
    @JsonIgnore
    private transient String base64;

    @SuppressWarnings("initialization.fields.uninitialized")
    @Deprecated
    public TopicAndPartition() {
    }

    public TopicAndPartition(String topic, int partition) {
        this.topic = topic;
        this.partition = partition;
    }

    public TopicAndPartition(IPartitionKey key) {
        this.topic = key.partitionTopic();
        this.partition = key.partitionIndex();
    }

    public String partitionTopic() {
        return topic;
    }

    public int partitionIndex() {
        return partition;
    }

    @Override
    public String toString() {
        return topic + "-" + partition;
    }

    @Override
    public int hashCode() {
        return toString().hashCode();
    }

    @Override
    public boolean equals(Object val) {
        if (val instanceof TopicPartition) {
            TopicPartition other = (TopicPartition)val;
            return this.partition == other.partition() &&
                   this.topic.equals(other.topic());
        }
        return false;
    }
}
