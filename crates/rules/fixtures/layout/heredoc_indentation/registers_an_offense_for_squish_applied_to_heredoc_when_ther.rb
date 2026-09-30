          def foo
            <<-RUBY2.squish
                something
^^^^^^^^^^^^^^^^^^^^^^^^^ Use 2 spaces for indentation in a heredoc by using `<<~` instead of `<<-`.
            RUBY2
          end
