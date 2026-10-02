unless FileUtils.options_of(:rm)
  FileUtils.mkdir_p(path)
end
if FileTest.executable?(path)
  FileUtils.remove(path)
end
