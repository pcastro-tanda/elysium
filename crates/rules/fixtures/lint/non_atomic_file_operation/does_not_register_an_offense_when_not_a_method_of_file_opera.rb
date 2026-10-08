unless FileUtils.exist?(path)
  FileUtils.options_of(:rm)
end
unless FileUtils.exist?(path)
  NotFile.remove(path)
end
