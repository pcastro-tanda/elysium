dest = src.map do |e|
  begin
    foo
    e * 2
  end
end
