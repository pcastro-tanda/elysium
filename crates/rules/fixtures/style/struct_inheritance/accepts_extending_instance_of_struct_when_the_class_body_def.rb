class Person < Struct.new(:first_name, :last_name)
  class Error < StandardError; end
end
