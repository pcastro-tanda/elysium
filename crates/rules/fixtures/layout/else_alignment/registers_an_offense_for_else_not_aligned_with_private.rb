private def test
          something
        rescue
          handling
        else
        ^^^^ Align `else` with `private`.
          something_else
        end
