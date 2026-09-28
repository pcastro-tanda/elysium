            def baz
              <<~'MSG'
              foo
^^^^^^^^^^^^^^^^^ Use 2 spaces for indentation in a heredoc.
    
                bar
              MSG
            end
