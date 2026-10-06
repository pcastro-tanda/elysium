def change_dir(&block)
  FileUtils.chdir("/var/run", &block)
end
