package com.tokera.ate.dao;

import com.fasterxml.jackson.annotation.JsonProperty;
import com.fasterxml.jackson.databind.annotation.JsonDeserialize;
import com.fasterxml.jackson.databind.annotation.JsonSerialize;
import com.tokera.ate.annotations.YamlTag;
import com.tokera.ate.common.StringTools;
import com.tokera.ate.io.api.IPartitionKey;
import com.tokera.ate.providers.PartitionKeySerializer;
import com.tokera.ate.providers.PuuidJsonDeserializer;
import com.tokera.ate.providers.PuuidJsonSerializer;
import com.tokera.ate.providers.PuuidSerializer;
import org.apache.commons.codec.binary.Base64;
import org.apache.commons.io.output.ByteArrayOutputStream;
import org.checkerframework.checker.nullness.qual.Nullable;

import java.io.DataOutputStream;
import java.io.IOException;
import java.io.Serializable;
import java.nio.ByteBuffer;
import java.util.Objects;
import java.util.UUID;

@YamlTag("puuid")
@JsonSerialize(using = PuuidJsonSerializer.class)
@JsonDeserialize(using = PuuidJsonDeserializer.class)
public final class PUUID implements Serializable, Comparable<PUUID> {
    private static final long serialVersionUID = -642512169720015696L;
    private Partition partition;
    private UUID id;

    public PUUID(String topic, int index, long mostSigBits, long leastSigBits) {
        this.partition = new Partition(topic, index);
        this.id = new UUID(mostSigBits, leastSigBits);
    }

    public PUUID(String topic, int index, UUID id) {
        this.partition = new Partition(topic, index);
        this.id = id;
    }

    public PUUID(IPartitionKey key, long mostSigBits, long leastSigBits) {
        this.partition = new Partition(key.partitionTopic(), key.partitionIndex());
        this.id = new UUID(mostSigBits, leastSigBits);
    }

    public PUUID(IPartitionKey key, UUID id) {
        this.partition = new Partition(key.partitionTopic(), key.partitionIndex());
        this.id = id;
    }

    public static PUUID from(IPartitionKey partitionKey, UUID id) {
        return new PUUID(partitionKey, id);
    }

    public class Partition implements IPartitionKey {
        private final String partitionTopic;
        private final int partitionIndex;

        public Partition(String partitionTopic, int partitionIndex) {
            this.partitionTopic = partitionTopic;
            this.partitionIndex = partitionIndex;
        }

        @Override
        public String partitionTopic() {
            return this.partitionTopic;
        }

        @Override
        public int partitionIndex() {
            return this.partitionIndex;
        }

        @Override
        public String toString() {
            return PartitionKeySerializer.toString(this);
        }

        @Override
        public int hashCode() {
            return PartitionKeySerializer.hashCode(this);
        }

        @Override
        public boolean equals(Object val) {
            return PartitionKeySerializer.equals(this, val);
        }
    }

    public IPartitionKey partition() {
        return this.partition;
    }

    public String partitionTopic() {
        return this.partition.partitionTopic;
    }

    public int partitionIndex() {
        return this.partition.partitionIndex;
    }

    public UUID id() {
        return this.id;
    }

    @Override
    public int compareTo(PUUID pid) {
        int diff = this.partitionTopic().compareTo(pid.partitionTopic());
        if (diff != 0) return diff;
        diff = Integer.compare(this.partitionIndex(), pid.partitionIndex());
        if (diff != 0) return diff;
        return this.id.compareTo(pid.id);
    }

    public int hashCode() {
        long hash = (this.partitionTopic() != null ? this.partitionTopic().hashCode() : 0) ^
                    Integer.hashCode(this.partitionIndex()) ^
                    (this.id != null ? this.id.hashCode() : 0);
        return (int)(hash >> 32) ^ (int)hash;
    }

    public boolean equals(Object other) {
        if (null != other && other.getClass() == PUUID.class) {
            PUUID pid = (PUUID)other;
            return Objects.equals(this.partitionTopic(), pid.partitionTopic()) &&
                    Objects.equals(this.partitionIndex(), pid.partitionIndex()) &&
                    Objects.equals(this.id, pid.id);
        } else {
            return false;
        }
    }

    @Override
    public String toString() {
        try {
            ByteArrayOutputStream stream = new ByteArrayOutputStream();
            DataOutputStream dos = new DataOutputStream(stream);
            String topic = this.partitionTopic();
            if (topic != null) {
                dos.writeShort(topic.length());
                dos.write(topic.getBytes());
            } else {
                dos.writeShort(0);
            }
            dos.writeInt(this.partitionIndex());
            UUID id = this.id();
            if (id != null) {
                dos.writeLong(id.getMostSignificantBits());
                dos.writeLong(id.getLeastSignificantBits());
            } else {
                dos.writeLong(0);
                dos.writeLong(0);
            }
            return Base64.encodeBase64URLSafeString(stream.toByteArray());
        } catch (IOException e) {
            throw new RuntimeException(e);
        }
    }

    public String print() {
        return this.partitionTopic() + ":" + this.partitionIndex() + ":" + this.id();
    }

    public static String toString(@Nullable PUUID pid) {
        if (pid == null) return "null";
        return pid.toString();
    }

    public static @Nullable PUUID parse(@Nullable String _val) {
        String val = StringTools.makeOneLineOrNull(_val);
        val = StringTools.specialParse(val);
        if (val == null || val.length() <= 0) return null;

        byte[] data = Base64.decodeBase64(val);
        ByteBuffer bb = ByteBuffer.wrap(data);

        String topic = null;
        int topicLen = bb.getShort();
        if (topicLen > 0) {
            byte[] topicBytes = new byte[topicLen];
            bb.get(topicBytes);
            topic = new String(topicBytes);
        }

        int index = bb.getInt();

        long mostSigBits = bb.getLong();
        long leastSigBits = bb.getLong();
        UUID id = null;
        if (mostSigBits != 0 && leastSigBits != 0) {
            id = new UUID(mostSigBits, leastSigBits);
        }

        return new PUUID(
                topic,
                index,
                id);
    }
}
