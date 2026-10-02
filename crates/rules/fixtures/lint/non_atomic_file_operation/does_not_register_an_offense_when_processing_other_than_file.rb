unless FileTest.exist?(path)
  FileUtils.makedirs(path)
  do_something
end

unless FileTest.exist?(path)
  do_something
  FileUtils.makedirs(path)
end
