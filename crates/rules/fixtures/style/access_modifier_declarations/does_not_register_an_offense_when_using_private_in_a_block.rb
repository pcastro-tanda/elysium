module MyModule
  singleton_methods.each { |method| private(method) }
end
