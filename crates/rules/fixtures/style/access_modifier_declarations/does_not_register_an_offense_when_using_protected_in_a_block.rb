module MyModule
  singleton_methods.each { |method| protected(method) }
end
