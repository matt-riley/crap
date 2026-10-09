local M = {}

function M.trivial() return 1 end                                  -- cc=1

function M.binary_ops(a, b) return a and b or a end                -- cc=3

function M.outer(a)                                                -- cc=2 (nested excluded)
  local inner = function(b) if b then return 1 else return 2 end end -- cc=2
  if a then return inner(a) end
  return 0
end

function M.loops(x)                                                -- cc=4
  for i = 1, 10 do end
  while x > 0 do x = x - 1 end
  repeat x = x - 1 until x == 0
  return x
end

function M.chained(x)                                              -- cc=4
  if x == 1 then return 1
  elseif x == 2 then return 2
  elseif x == 3 then return 3
  else return 4 end
end

return M
