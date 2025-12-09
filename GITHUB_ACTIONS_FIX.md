# GitHub Actions 工作流修复建议

## 问题分析

根据错误信息，问题出在剥离二进制文件的步骤。工作流试图剥离位于 `target/${{ matrix.target }}/release/rsync` 的文件，但该文件不存在。

## 解决方案

### 1. 添加构建输出验证步骤

在构建步骤之后添加一个验证步骤，以确认二进制文件的实际位置：

```yaml
- name: Check build output
  working-directory: ./rsync
  run: |
    echo "Checking target directory structure:"
    find target -type f -name "rsync" | head -10
    echo "Target directory contents:"
    ls -la target/
    if [ -d "target/${{ matrix.target }}" ]; then
      echo "Target-specific directory contents:"
      ls -la target/${{ matrix.target }}/ || true
      if [ -d "target/${{ matrix.target }}/release" ]; then
        ls -la target/${{ matrix.target }}/release/ || true
      fi
    fi
    echo "Release directory contents:"
    ls -la target/release/ || true
```

### 2. 更新剥离步骤

根据实际的构建输出位置更新剥离步骤：

```yaml
- name: Strip binary (Linux)
  if: runner.os == 'Linux'
  working-directory: ./rsync
  run: |
    # 确定二进制文件的实际位置
    if [ -f "target/${{ matrix.target }}/release/${{ matrix.artifact_name }}" ]; then
      BINARY_PATH="target/${{ matrix.target }}/release/${{ matrix.artifact_name }}"
    elif [ -f "target/release/${{ matrix.artifact_name }}" ]; then
      BINARY_PATH="target/release/${{ matrix.artifact_name }}"
    else
      echo "Error: Binary file not found"
      exit 1
    fi

    echo "Stripping binary at: $BINARY_PATH"
    if [ "${{ matrix.target }}" = "aarch64-unknown-linux-gnu" ]; then
      aarch64-linux-gnu-strip "$BINARY_PATH"
    else
      strip "$BINARY_PATH"
    fi
```

### 3. 更新复制步骤

同样更新复制步骤以使用正确的路径：

```yaml
- name: Create release directory structure
  run: |
    mkdir -p rsde
    # 确定二进制文件的实际位置
    if [ -f "rsync/target/${{ matrix.target }}/release/${{ matrix.artifact_name }}" ]; then
      cp "rsync/target/${{ matrix.target }}/release/${{ matrix.artifact_name }}" rsde/
    elif [ -f "rsync/target/release/${{ matrix.artifact_name }}" ]; then
      cp "rsync/target/release/${{ matrix.artifact_name }}" rsde/
    else
      echo "Error: Binary file not found for copying"
      exit 1
    fi
    cp rsync/README.md rsde/ 2>/dev/null || echo "# Rsync" > rsde/README.md
    cp rsync/example.toml rsde/ 2>/dev/null || true
```

## 总结

通过添加验证步骤和动态确定二进制文件位置，可以确保工作流无论构建产物位于何处都能正确处理。