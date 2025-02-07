SRC != find src -name '*.rs'

all: ppa6

clean:
	rm -rf target
	rm -f ppa6

run: ppa6
	doas ./ppa6

ppa6: ${SRC}
	cargo build
	cp -f target/debug/ppa6 .
