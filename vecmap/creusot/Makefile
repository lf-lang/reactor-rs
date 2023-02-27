all: creusot ide

creusot:
	cargo creusot -- --features=contracts
	-mv -u target/debug/*.mlcfg .

ide: *.mlcfg
	./ide $<

clean:
	rm *.mlcfg
	cargo clean

.PHONY: all creusot ide clean
