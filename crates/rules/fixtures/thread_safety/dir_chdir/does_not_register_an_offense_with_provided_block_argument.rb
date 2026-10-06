def change_dir(&block)
  Dir.chdir("/var/run", &block)
end
