def method_missing(name, ...)
  @proxy.call(name, ...)
end
