def foo(bar)
  begin
  rescue StandardError => _
  end
  bar[:baz] = true
end
