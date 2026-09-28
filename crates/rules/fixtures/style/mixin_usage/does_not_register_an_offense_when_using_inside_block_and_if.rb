klass.class_eval do
  include M1
  include M2 if defined?(M)
end
