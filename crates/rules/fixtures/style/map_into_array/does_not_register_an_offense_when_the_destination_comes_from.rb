[].tap do |dest|
  do_something(dest)
  src.each { |e| dest << e * 2 }
end
