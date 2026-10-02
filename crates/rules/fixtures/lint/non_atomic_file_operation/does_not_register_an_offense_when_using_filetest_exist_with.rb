if FileTest.exist?(path)
  FileUtils.mkdir(path)
else
  do_something
end
