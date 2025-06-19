#!/bin/bash

# the network topology in the homework
nodes=(switch host1 host2)

links=( switch switch-eth0 66:77:88:00:00:01 host1 host1-eth0 66:77:88:00:00:02 \
    switch switch-eth1 66:77:88:00:00:03 host2 host2-eth0 66:77:88:00:00:04 )

# IP address configuration: interface ip_address

ips=(host1-eth0 10.0.0.1/24 \
    host2-eth0 10.0.0.2/24 )

arp=(host1 66:77:88:00:00:02 10.0.0.1 \
    host2 66:77:88:00:00:04 10.0.0.2 )

client_port=30001
server_port=20001

setup() {
    # create and start the nodes
    for node in ${nodes[@]}; do
        echo "Creating and starting $node"
        docker container create --cap-add NET_ADMIN --name $node -v $(pwd):/app node 
        docker container start $node
    done

    # create veth links between containers
    for ((i=0; i<${#links[@]}; i+=6)); do
        container1=${links[i]}
        veth1=${links[i+1]}
        mac1=${links[i+2]}
        container2=${links[i+3]}
        veth2=${links[i+4]}
        mac2=${links[i+5]}

        echo "Creating link between $container1($veth1) and $container2($veth2)"
        
        # Create veth pair
        # echo ip link add $veth1 type veth peer name $veth2
        sudo ip link add $veth1 type veth peer name $veth2
        
        # Set MAC addresses
        sudo ip link set $veth1 address $mac1
        sudo ip link set $veth2 address $mac2
        
        # Move veth interfaces to containers
        sudo ip link set $veth1 netns $(docker inspect -f '{{.State.Pid}}' $container1)
        sudo ip link set $veth2 netns $(docker inspect -f '{{.State.Pid}}' $container2)

        # set interfaces promisc mode
        sudo docker exec $container1 ip link set $veth1 promisc on
        sudo docker exec $container2 ip link set $veth2 promisc on

        # Bring interfaces up
        sudo docker exec $container1 ip link set $veth1 up
        sudo docker exec $container2 ip link set $veth2 up
    done

    # configure IP addresses
    for ((i=0; i<${#ips[@]}; i+=2)); do
        interface=${ips[i]}
        ip_addr=${ips[i+1]}
        container=${interface%-*}  # extract container name from interface
        
        echo "Configuring $interface with $ip_addr in $container"
        sudo docker exec $container ip addr add $ip_addr dev $interface
    done
    # configure ARP mappings in host containers
    for node in ${nodes[@]}; do
        if [[ $node == host* ]]; then
            echo "Configuring ARP in $node"
            for ((i=0; i<${#arp[@]}; i+=3)); do
                container=${arp[i]}
                mac=${arp[i+1]}
                ip_addr=${arp[i+2]%/*}  # remove /24 suffix
                if [[ $container != $node ]]; then
                    sudo docker exec $node arp -s $ip_addr $mac
                fi
            done
        fi
    done
}

clean() {
    # remove all containers
    for node in ${nodes[@]}; do
        echo remove $node
        docker container rm -f $node
    done
}


links2=( host1 host1-eth0 66:77:88:00:00:02 host2 host2-eth0 66:77:88:00:00:04 )


setup2() {
    # create and start the nodes
    for node in ${nodes[@]}; do
        echo "Creating and starting $node"
        docker container create --cap-add NET_ADMIN --name $node -v $(pwd):/app node 
        docker container start $node
    done

    # create veth links between containers
    for ((i=0; i<${#links2[@]}; i+=6)); do
        container1=${links2[i]}
        veth1=${links2[i+1]}
        mac1=${links2[i+2]}
        container2=${links2[i+3]}
        veth2=${links2[i+4]}
        mac2=${links2[i+5]}

        echo "Creating link between $container1($veth1) and $container2($veth2)"
        
        # Create veth pair
        # echo ip link add $veth1 type veth peer name $veth2
        sudo ip link add $veth1 type veth peer name $veth2
        
        # Set MAC addresses
        sudo ip link set $veth1 address $mac1
        sudo ip link set $veth2 address $mac2
        
        # Move veth interfaces to containers
        sudo ip link set $veth1 netns $(docker inspect -f '{{.State.Pid}}' $container1)
        sudo ip link set $veth2 netns $(docker inspect -f '{{.State.Pid}}' $container2)
        
        # Bring interfaces up
        sudo docker exec $container1 ip link set $veth1 up
        sudo docker exec $container2 ip link set $veth2 up
    done

    # configure IP addresses
    for ((i=0; i<${#ips[@]}; i+=2)); do
        interface=${ips[i]}
        ip_addr=${ips[i+1]}
        container=${interface%-*}  # extract container name from interface
        
        echo "Configuring $interface with $ip_addr in $container"
        sudo docker exec $container ip addr add $ip_addr dev $interface
    done
    # configure ARP mappings in host containers
    for node in ${nodes[@]}; do
        if [[ $node == host* ]]; then
            echo "Configuring ARP in $node"
            for ((i=0; i<${#arp[@]}; i+=3)); do
                container=${arp[i]}
                mac=${arp[i+1]}
                ip_addr=${arp[i+2]%/*}  # remove /24 suffix
                if [[ $container != $node ]]; then
                    sudo docker exec $node arp -s $ip_addr $mac
                fi
            done
        fi
    done
}

# handle command line arguments
case "$1" in
    "setup")
        setup
        ;;
    "setup2")
        setup2
        ;;
    "clean")
        clean
        ;;
    *)
        echo "Usage: $0 {setup|setup2|clean}"
        exit 1
        ;;
esac




