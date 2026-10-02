Dir.mkdir(path, 0o0755) unless Dir.exist?(path)
                        ^^^^^^^^^^^^^^^^^^^^^^^ Remove unnecessary existence check `Dir.exist?`.
^^^^^^^^^^^^^^^^^^^^^^^ Use atomic file operation method `FileUtils.mkdir_p`.
