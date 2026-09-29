format(<<-HEREDOC, foo: bar)
foo %<named>u + bar %{template}
                    ^^^^^^^^^^^ Prefer annotated tokens (like `%<foo>s`) over template tokens (like `%{foo}`).
HEREDOC
