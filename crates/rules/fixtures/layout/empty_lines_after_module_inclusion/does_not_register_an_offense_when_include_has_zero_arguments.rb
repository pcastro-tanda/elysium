class Foo
  includes = [include, sdk_include].compact.map do |inc|
    inc + "blah"
  end.join(' ')
end
