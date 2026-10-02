File.binwrite(filename, <<~EOS.gsub(/^/, ''))
    content
  EOS
