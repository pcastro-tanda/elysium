def foo(*args)
  Ractor.new(*args) do |*args|
  end
end
