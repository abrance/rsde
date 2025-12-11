CURRENT_PATH:=$(realpath $(firstword $(MAKEFILE_LIST)))
ROOT_DIR:=$(shell dirname $(shell dirname $(CURRENT_PATH)))

MANIFEST_DIR = $(ROOT_DIR)/manifest
TRAIN_DATA_DIR = $(ROOT_DIR)/train_data

REMOTE_CHI_TRAIN_DATA_URL = "https://raw.githubusercontent.com/tesseract-ocr/tessdata/main/chi_sim.traineddata"
REMOTE_ENG_TRAIN_DATA_URL = "https://raw.githubusercontent.com/tesseract-ocr/tessdata/main/eng.traineddata"

.PHONY: dep-install
dep-install:
	# Install Tesseract OCR library(debian)
	sudo apt install -y tesseract-ocr libtesseract-dev

.PHONY: traindata-deploy
traindata-deploy:
	# Deploy Tesseract OCR trained data files
	echo ${CURRENT_PATH}
	echo ${ROOT_DIR}
	echo ${MANIFEST_DIR}
	echo ${TRAIN_DATA_DIR}
	mkdir -p ${TRAIN_DATA_DIR}
	cd ${TRAIN_DATA_DIR} && wget ${REMOTE_CHI_TRAIN_DATA_URL} && wget ${REMOTE_ENG_TRAIN_DATA_URL}
	echo "Tesseract OCR trained data files deployed."