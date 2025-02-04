ROOT := /Users/ethan/gh/cs6120
BRIL := /Users/ethan/gh/bril

.PHONY: default
default:
	@echo "Please specify a test target"
	@echo
	@echo "Use 'make help' for help"

.PHONY: build_cfg
build_cfg:	## Test build-cfg
	make test_equivalence \
		EQUIV_DIR="lesson2" \
		EQUIV_NAME="build-cfg" \
		EQUIV_CMD="${ROOT}/target/debug/build-cfg --mode passthrough" \
		EQUIV_PIPE="bril2json"

EQUIV_DIR := .
EQUIV_NAME := _
EQUIV_CMD := _
EQUIV_PIPE := cat
EQUIV_FLAGS := --exclude benchmarks/float/cordic.bril --exclude benchmarks/mem/cordic.bril
.PHONY: test_equivalence
test_equivalence:
	cd "${EQUIV_DIR}" && \
	python3 "${ROOT}/lesson2/test/check_equivalence.py" \
		"${EQUIV_NAME}" \
		"${EQUIV_CMD}" \
		"${EQUIV_PIPE}" \
		${BRIL}/benchmarks/**/*.bril \
		${EQUIV_FLAGS}

# https://stackoverflow.com/questions/8889035/how-to-document-a-makefile
help:     	## Shows this help.
	@sed -ne '/@sed/!s/## //p' ${MAKEFILE_LIST}
