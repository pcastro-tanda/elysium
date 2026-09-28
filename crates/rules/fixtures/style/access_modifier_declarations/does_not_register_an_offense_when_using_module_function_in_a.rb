module MyModule
  singleton_methods.each { |method| module_function(method) }
end
