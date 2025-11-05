#!/usr/bin/env python3
"""
Kafka Client for Topic Management
This script provides functionality to create, list, and delete Kafka topics.
"""

import argparse
import sys

from kafka.admin import KafkaAdminClient, NewTopic
from kafka.errors import (
    KafkaError,
    TopicAlreadyExistsError,
    UnknownTopicOrPartitionError,
)


class KafkaTopicManager:
    """Manages Kafka topics including create, list, and delete operations."""

    def __init__(
        self,
        bootstrap_servers="localhost:9092",
        timeout_ms=30000,
        security_protocol="PLAINTEXT",
        sasl_mechanism=None,
        sasl_username=None,
        sasl_password=None,
    ):
        """
        Initialize Kafka Admin Client.

        Args:
            bootstrap_servers (str): Kafka broker addresses, default is 'localhost:9092'
            timeout_ms (int): Connection timeout in milliseconds
            security_protocol (str): Security protocol (PLAINTEXT, SASL_PLAINTEXT, SASL_SSL, SSL)
            sasl_mechanism (str): SASL mechanism (PLAIN, SCRAM-SHA-256, SCRAM-SHA-512, GSSAPI)
            sasl_username (str): SASL username
            sasl_password (str): SASL password
        """
        try:
            auth_mode = "认证" if security_protocol != "PLAINTEXT" else "无认证"
            print(f"Connecting to Kafka at {bootstrap_servers} ({auth_mode})...")

            # 基础配置
            config = {
                "bootstrap_servers": bootstrap_servers,
                "client_id": "kafka-topic-manager",
                "request_timeout_ms": timeout_ms,
                "api_version_auto_timeout_ms": timeout_ms,
                "connections_max_idle_ms": 540000,
                "metadata_max_age_ms": 300000,
            }

            # SASL 认证配置
            if security_protocol != "PLAINTEXT":
                config["security_protocol"] = security_protocol

                if sasl_mechanism:
                    config["sasl_mechanism"] = sasl_mechanism

                if sasl_username and sasl_password:
                    config["sasl_plain_username"] = sasl_username
                    config["sasl_plain_password"] = sasl_password
                    print(
                        f"  Using SASL authentication: {sasl_mechanism} (user: {sasl_username})"
                    )

            self.admin_client = KafkaAdminClient(**config)
            print(f"✓ Successfully connected to Kafka at {bootstrap_servers}")

        except Exception as e:
            print(f"✗ Failed to connect to Kafka: {e}")
            print(f"\n提示:")
            print(f"  1. 确保 Kafka 服务器正在运行")
            print(f"  2. 如果使用认证，检查用户名和密码是否正确")
            print(f"  3. 运行 'python test_connection.py' 进行诊断")
            print(f"  4. 如果没有 Kafka，可以使用 'docker-compose up -d' 启动")
            sys.exit(1)

    def create_topic(self, topic_name, num_partitions=1, replication_factor=1):
        """
        Create a new Kafka topic.

        Args:
            topic_name (str): Name of the topic to create
            num_partitions (int): Number of partitions for the topic
            replication_factor (int): Replication factor for the topic

        Returns:
            bool: True if successful, False otherwise
        """
        try:
            topic = NewTopic(
                name=topic_name,
                num_partitions=num_partitions,
                replication_factor=replication_factor,
            )

            result = self.admin_client.create_topics([topic], validate_only=False)
            print(f"✓ Topic '{topic_name}' created successfully")
            print(f"  - Partitions: {num_partitions}")
            print(f"  - Replication Factor: {replication_factor}")
            return True

        except TopicAlreadyExistsError:
            print(f"✗ Topic '{topic_name}' already exists")
            return False
        except Exception as e:
            print(f"✗ Failed to create topic '{topic_name}': {e}")
            return False

    def list_topics(self):
        """
        List all available Kafka topics.

        Returns:
            list: List of topic names
        """
        try:
            topics = self.admin_client.list_topics()
            print(f"\n{'='*60}")
            print(f"Available Kafka Topics ({len(topics)} total):")
            print(f"{'='*60}")

            if topics:
                for idx, topic in enumerate(sorted(topics), 1):
                    print(f"{idx:3d}. {topic}")
            else:
                print("No topics found")

            print(f"{'='*60}\n")
            return topics

        except Exception as e:
            print(f"✗ Failed to list topics: {e}")
            return []

    def delete_topic(self, topic_name):
        """
        Delete a Kafka topic.

        Args:
            topic_name (str): Name of the topic to delete

        Returns:
            bool: True if successful, False otherwise
        """
        try:
            result = self.admin_client.delete_topics([topic_name])
            print(f"✓ Topic '{topic_name}' deleted successfully")
            return True

        except UnknownTopicOrPartitionError:
            print(f"✗ Topic '{topic_name}' does not exist")
            return False
        except Exception as e:
            print(f"✗ Failed to delete topic '{topic_name}': {e}")
            return False

    def get_topic_metadata(self, topic_name):
        """
        Get metadata for a specific topic.

        Args:
            topic_name (str): Name of the topic

        Returns:
            dict: Topic metadata
        """
        try:
            metadata = self.admin_client.describe_topics([topic_name])
            print(f"\nTopic Metadata for '{topic_name}':")
            print(f"{'='*60}")
            for topic_metadata in metadata:
                print(f"Topic: {topic_metadata['topic']}")
                print(f"Partitions: {len(topic_metadata['partitions'])}")
                for partition in topic_metadata["partitions"]:
                    print(
                        f"  Partition {partition['partition']}: Leader={partition['leader']}, "
                        f"Replicas={partition['replicas']}, ISR={partition['isr']}"
                    )
            print(f"{'='*60}\n")
            return metadata
        except Exception as e:
            print(f"✗ Failed to get metadata for topic '{topic_name}': {e}")
            return None

    def close(self):
        """Close the admin client connection."""
        try:
            self.admin_client.close()
            print("✓ Kafka connection closed")
        except Exception as e:
            print(f"✗ Error closing connection: {e}")


def main():
    """Main function to handle command-line arguments and execute operations."""
    parser = argparse.ArgumentParser(
        description="Kafka Topic Manager - Create, List, and Delete Topics",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  # List all topics
  python kafka.py list
  
  # Create a new topic
  python kafka.py create --topic my-topic --partitions 3 --replication-factor 2
  
  # Delete a topic
  python kafka.py delete --topic my-topic
  
  # Get topic metadata
  python kafka.py metadata --topic my-topic
  
  # Use custom Kafka server
  python kafka.py list --server kafka-broker:9092
        """,
    )

    parser.add_argument(
        "operation",
        choices=["create", "list", "delete", "metadata"],
        help="Operation to perform: create, list, delete, or metadata",
    )

    parser.add_argument(
        "--server",
        default="localhost:9092",
        help="Kafka bootstrap server address (default: localhost:9092)",
    )

    parser.add_argument(
        "--topic",
        help="Topic name (required for create, delete, and metadata operations)",
    )

    parser.add_argument(
        "--partitions",
        type=int,
        default=1,
        help="Number of partitions for topic creation (default: 1)",
    )

    parser.add_argument(
        "--replication-factor",
        type=int,
        default=1,
        help="Replication factor for topic creation (default: 1)",
    )

    # SASL 认证参数
    parser.add_argument(
        "--security-protocol",
        default="PLAINTEXT",
        choices=["PLAINTEXT", "SASL_PLAINTEXT", "SASL_SSL", "SSL"],
        help="Security protocol (default: PLAINTEXT)",
    )

    parser.add_argument(
        "--sasl-mechanism",
        choices=["PLAIN", "SCRAM-SHA-256", "SCRAM-SHA-512", "GSSAPI"],
        help="SASL mechanism (e.g., PLAIN, SCRAM-SHA-256)",
    )

    parser.add_argument(
        "--sasl-username",
        help="SASL username for authentication",
    )

    parser.add_argument(
        "--sasl-password",
        help="SASL password for authentication",
    )

    args = parser.parse_args()

    # Validate arguments
    if args.operation in ["create", "delete", "metadata"] and not args.topic:
        parser.error(f"--topic is required for '{args.operation}' operation")

    # Validate SASL configuration
    if args.security_protocol != "PLAINTEXT":
        if not args.sasl_mechanism:
            parser.error(
                f"--sasl-mechanism is required when using {args.security_protocol}"
            )
        if args.sasl_mechanism in ["PLAIN", "SCRAM-SHA-256", "SCRAM-SHA-512"]:
            if not args.sasl_username or not args.sasl_password:
                parser.error(
                    f"--sasl-username and --sasl-password are required for {args.sasl_mechanism}"
                )

    # Initialize Kafka manager
    print(f"\n{'='*60}")
    print("Kafka Topic Manager")
    print(f"{'='*60}\n")

    manager = KafkaTopicManager(
        bootstrap_servers=args.server,
        security_protocol=args.security_protocol,
        sasl_mechanism=args.sasl_mechanism,
        sasl_username=args.sasl_username,
        sasl_password=args.sasl_password,
    )

    try:
        # Execute the requested operation
        if args.operation == "list":
            manager.list_topics()

        elif args.operation == "create":
            manager.create_topic(
                topic_name=args.topic,
                num_partitions=args.partitions,
                replication_factor=args.replication_factor,
            )

        elif args.operation == "delete":
            manager.delete_topic(args.topic)

        elif args.operation == "metadata":
            manager.get_topic_metadata(args.topic)

    finally:
        manager.close()
        print()


if __name__ == "__main__":
    main()
