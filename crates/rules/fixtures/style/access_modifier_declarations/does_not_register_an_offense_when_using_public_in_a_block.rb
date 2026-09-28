module MyModule
  singleton_methods.each { |method| public(method) }
end
