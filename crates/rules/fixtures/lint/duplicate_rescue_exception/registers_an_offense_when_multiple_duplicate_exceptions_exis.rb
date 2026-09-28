begin
  something
rescue FirstError
rescue SecondError
rescue FirstError
       ^^^^^^^^^^ Duplicate `rescue` exception detected.
rescue SecondError
       ^^^^^^^^^^^ Duplicate `rescue` exception detected.
end
