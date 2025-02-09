PREFIX = /usr/local

SRC != find ppa6 ppa6-print -name '*.rs'

all: bin/ppa6-print

clean:
	rm -rf target
	rm -f ppa6-print

install: bin/ppa6-print
	mkdir -p ${DESTDIR}${PREFIX}/bin
	cp -f bin/* ${DESTDIR}${PREFIX}/bin/

bin/ppa6-print: ${SRC}
	mkdir -p bin
	cargo build --release -p ppa6-print
	cp -f target/release/ppa6-print bin/


