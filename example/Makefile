all: elf dump

.PHONY: elf
elf: $(patsubst %/go.mod,bin/%.elf,$(wildcard */go.mod))

.PHONY: dump
dump: $(patsubst %/go.mod,bin/%.dump,$(wildcard */go.mod))

# .PHONY: ex-rs
# ex-rs:
# 	cd hello-rs && \
# 		RUSTFLAGS='-C link-arg=-no-pie -C target-cpu=mips32 -C target-feature=-mips32r2,-fpxx,-nooddspreg,+mips32,+crt-static,+soft-float' \
# 			cross build --release --target ../mips-unknown-linux-gnu.json -Z build-std -Z build-std-features=panic-unwind && \
# 		cp ../../target/mips-unknown-linux-gnu/release/hello-rs ../bin/hello-rs.elf

bin:
	mkdir bin

# take any directory with a go mod, and build an ELF
# verify output with: readelf -h bin/<name>.elf
# result is mips32, big endian, R3000
bin/%.elf: bin
	cd $(@:bin/%.elf=%) && GOOS=linux GOARCH=mips GOMIPS=softfloat go build -o ../$@ .

# take any ELF and dump it
# TODO: currently have the little-endian toolchain, but should use the big-endian one. The -EB compat flag works though.
bin/%.dump: bin/%.elf
	mipsel-linux-gnu-objdump -D --disassembler-options=no-aliases --wide --source -m mips:3000 -EB $(@:%.dump=%.elf) > $@
