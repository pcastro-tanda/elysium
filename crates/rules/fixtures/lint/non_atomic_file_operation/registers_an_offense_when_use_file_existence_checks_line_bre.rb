FileUtils.mkdir(path) unless
                      ^^^^^^ Remove unnecessary existence check `FileTest.exist?`.
^^^^^^^^^^^^^^^^^^^^^ Use atomic file operation method `FileUtils.mkdir_p`.
                      FileTest.exist?(path)
