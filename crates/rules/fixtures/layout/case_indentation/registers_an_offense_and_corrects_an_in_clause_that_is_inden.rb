case a
    in 0 then return
    ^^ Indent `in` as deep as `case`.
    else
        case b
         in 1 then return
         ^^ Indent `in` as deep as `case`.
        end
end
