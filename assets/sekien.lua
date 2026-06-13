-- sekien.lua — pandoc Lua filter for sekien-pandoc
--
-- Converts RawBlock("html", <svg...>) nodes produced by sekien-pandoc into
-- Image nodes pointing to temporary SVG files. This allows PDF engines that
-- drop raw HTML (e.g. typst, pdflatex) to include the SVG.
--
-- Usage:
--   pandoc input.md -o output.pdf \
--     --pdf-engine=typst \
--     --filter sekien-pandoc \
--     --lua-filter <(sekien-pandoc --print-lua-filter)
--
-- Tempfile path:
--   typst は image パスを typst root からの相対パスとして解決する。
--   pandoc が typst を呼ぶ際の root はデフォルトで CWD になるため、
--   /tmp/foo.svg は <CWD>/tmp/foo.svg として解決されてしまう。
--
--   これを避けるため、pandoc に --pdf-engine-opt=--root=/ を渡すことで
--   typst の root をファイルシステムルートにする。そのうえで SVG を /tmp/ に
--   書き出せば pandoc 実行後に OS が自動的に掃除する:
--
--     pandoc input.md -o output.pdf \
--       --pdf-engine=typst \
--       --pdf-engine-opt=--root=/ \
--       --filter sekien-pandoc \
--       --lua-filter <(sekien-pandoc --print-lua-filter) \
--       -V mainfont="Noto Sans"
--
--   --root=/ を渡さない場合は CWD に sekien-pandoc-*.svg が残る。
--   その場合は .gitignore 等で管理すること。
--
-- Tempfile lifetime:
--   pandoc は filter 完了後の PDF 生成フェーズに image を読みに行くため
--   filter 側からは削除できない。

local function random_hex8()
  local rand = io.open("/dev/urandom", "rb")
  if rand then
    local bytes = rand:read(8)
    rand:close()
    if bytes and #bytes == 8 then
      return bytes:gsub(".", function(c)
        return string.format("%02x", c:byte())
      end)
    end
  end
  -- fallback: tmpname の basename から hex 相当の文字列を取り出す
  local tmp = os.tmpname()
  local base = tmp:match("[^/\\]+$") or tmp
  return base:gsub("[^%w]", ""):sub(1, 16)
end

local function svg_tmp_path()
  local tmpdir = (os.getenv("TMPDIR") or "/tmp"):gsub("/$", "")
  return tmpdir .. "/sekien-pandoc-" .. random_hex8() .. ".svg"
end

function RawBlock(el)
  if el.format ~= "html" then return nil end
  if not el.text:match("^%s*<svg") then return nil end

  local path = svg_tmp_path()

  local f, err = io.open(path, "w")
  if not f then
    io.stderr:write("sekien.lua: cannot open " .. path .. ": " .. tostring(err) .. "\n")
    return nil
  end
  f:write(el.text)
  f:close()

  -- pandoc 3.x: Image(caption, src[, title[, attr]])
  -- caption は pandoc.Inlines({}) が必要 ({} では型変換に失敗する)
  return pandoc.Para({ pandoc.Image(pandoc.Inlines({}), path, "") })
end
