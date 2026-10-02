Dir.mkdir(path) unless Dir.exist?(path)
                ^^^^^^^^^^^^^^^^^^^^^^^ Remove unnecessary existence check `Dir.exist?`.
^^^^^^^^^^^^^^^ Use atomic file operation method `FileUtils.mkdir_p`.
