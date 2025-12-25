# WebServer Frontend Makefile

FRONTEND_DIR = frontend
DIST_DIR = $(FRONTEND_DIR)/dist
NODE_MODULES = $(FRONTEND_DIR)/node_modules

.PHONY: all build install dev clean help

# 默认目标
all: build

# 构建前端 dist 资源
build: install
	@echo "🔨 构建前端..."
	cd $(FRONTEND_DIR) && npm run build
	@echo "✅ 前端构建完成: $(DIST_DIR)"

# 安装依赖
install:
	@if [ ! -d "$(NODE_MODULES)" ]; then \
		echo "📥 安装前端依赖..."; \
		cd $(FRONTEND_DIR) && npm install; \
	else \
		echo "✅ 依赖已存在"; \
	fi

# 启动开发服务器
dev: install
	@echo "🚀 启动前端开发服务器..."
	cd $(FRONTEND_DIR) && npm run dev

# 预览生产构建
preview: build
	@echo "👀 预览生产构建..."
	cd $(FRONTEND_DIR) && npm run preview

# 清理构建产物
clean:
	@echo "🧹 清理构建产物..."
	rm -rf $(DIST_DIR)
	@echo "✅ 清理完成"

# 完全清理（包括依赖）
distclean: clean
	@echo "🧹 清理依赖..."
	rm -rf $(NODE_MODULES)
	@echo "✅ 完全清理完成"

# 帮助信息
help:
	@echo "WebServer Frontend Makefile"
	@echo ""
	@echo "可用目标："
	@echo "  make build      - 构建前端 dist 资源"
	@echo "  make install    - 安装前端依赖"
	@echo "  make dev        - 启动开发服务器"
	@echo "  make preview    - 预览生产构建"
	@echo "  make clean      - 清理构建产物"
	@echo "  make distclean  - 完全清理（包括依赖）"
	@echo "  make help       - 显示此帮助信息"
