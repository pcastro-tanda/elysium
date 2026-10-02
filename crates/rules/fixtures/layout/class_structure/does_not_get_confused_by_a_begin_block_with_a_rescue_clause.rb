class Foo
  begin
    require 'optional_dependency'
  rescue LoadError
    nil
  end
end
