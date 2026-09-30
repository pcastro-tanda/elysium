y = case a
    in 0 then raise
    ^^ Indent `in` one step more than `case`.
    in 0 then return
    ^^ Indent `in` one step more than `case`.
      z = case b
          in 1 then return
          ^^ Indent `in` one step more than `case`.
          in 1 then raise
          ^^ Indent `in` one step more than `case`.
          end
    end
case c
in 2 then encoding
^^ Indent `in` one step more than `case`.
end
