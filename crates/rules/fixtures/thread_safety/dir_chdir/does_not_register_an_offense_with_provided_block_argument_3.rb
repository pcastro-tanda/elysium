def change_dir(&block)
  FileUtils.cd("/var/run", &block)
end
