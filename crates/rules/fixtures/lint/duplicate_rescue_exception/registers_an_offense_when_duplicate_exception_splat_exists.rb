begin
  something
rescue *ERRORS
rescue SecondError, *ERRORS
                    ^^^^^^^ Duplicate `rescue` exception detected.
end
