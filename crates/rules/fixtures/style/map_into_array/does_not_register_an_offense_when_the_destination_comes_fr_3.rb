{}.tap do |dest|
  src.each { |e| dest << e * 2 }
end
