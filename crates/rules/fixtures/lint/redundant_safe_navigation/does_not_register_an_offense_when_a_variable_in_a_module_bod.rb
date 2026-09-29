foo.each do |v|
  if v
    module M
      v = bar
      v&.baz
    end
  end
end
