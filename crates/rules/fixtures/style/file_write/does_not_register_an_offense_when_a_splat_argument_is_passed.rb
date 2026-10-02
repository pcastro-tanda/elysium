File.open(filename, 'w') do |f|
  f.write(*objects)
end
