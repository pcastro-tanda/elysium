begin
  something
rescue FirstError
rescue SecondError, FirstError
                    ^^^^^^^^^^ Duplicate `rescue` exception detected.
rescue
end
