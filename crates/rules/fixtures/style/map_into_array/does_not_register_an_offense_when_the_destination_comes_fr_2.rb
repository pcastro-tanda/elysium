[1, 2, 3].tap do |dest|
  src.each { |e| dest << e * 2 }
end
