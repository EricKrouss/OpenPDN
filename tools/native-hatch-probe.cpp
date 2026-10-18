// Numerical oracle for the 8x8 hatch tiles used by the original HatchBrush.
#include <windows.h>
#include <gdiplus.h>
#include <cstdio>
#include <fstream>
#include <vector>
using namespace Gdiplus;
// Wine lacks this API. Native callers treat it as an integer DPI multiplier.
// The fixture is explicitly an unscaled 96-DPI bitmap.
static UINT WINAPI bitmap_dpi_scale(HDC) { return 1; }
int main(int argc,char** argv) {
    if(argc!=3) return 1;
    char path[MAX_PATH]; if(!GetFullPathNameA(argv[1],MAX_PATH,path,nullptr)) return 1;
    auto dll=LoadLibraryExA(path,nullptr,LOAD_WITH_ALTERED_SEARCH_PATH); if(!dll) return 1;
#define API(name) auto name=reinterpret_cast<decltype(&Gdiplus::DllExports::name)>(GetProcAddress(dll,#name)); if(!name) return 1
    API(GdipCreateBitmapFromScan0); API(GdipGetImageGraphicsContext); API(GdipSetCompositingMode);
    API(GdipCreateHatchBrush); API(GdipFillRectangleI); API(GdipDeleteBrush); API(GdipDeleteGraphics); API(GdipDisposeImage);
    auto startup=reinterpret_cast<decltype(&GdiplusStartup)>(GetProcAddress(dll,"GdiplusStartup"));
    auto shutdown=reinterpret_cast<decltype(&GdiplusShutdown)>(GetProcAddress(dll,"GdiplusShutdown"));
    // Exact 10.0.19041.329 IAT slot, guarded by the Python driver's DLL hash.
    auto dpi_slot=reinterpret_cast<void**>(reinterpret_cast<unsigned char*>(dll)+0x14f750);
    DWORD protection; if(!VirtualProtect(dpi_slot,sizeof(void*),PAGE_READWRITE,&protection)) return 1;
    *dpi_slot=reinterpret_cast<void*>(&bitmap_dpi_scale);
    VirtualProtect(dpi_slot,sizeof(void*),protection,&protection);
    ULONG_PTR token; GdiplusStartupInput input; if(!startup||!shutdown||startup(&token,&input,nullptr)!=Ok) return 1;
    std::ofstream out(argv[2]); out<<'[';
    const ARGB fg[]={0xff000000,0x80c8281e,0x00ff00ff,0xff28d25a};
    const ARGB bg[]={0xffffffff,0xff1e28c8,0xff4678d2,0x802850dc};
    for(int style=0;style<=52;style++) {
        if(style)out<<',';out<<"{\"style\":"<<style<<",\"samples\":[";
        for(int variant=0;variant<4;variant++) {
            std::vector<unsigned char> data(8*8*4); GpBitmap* bitmap=nullptr; GpGraphics* graphics=nullptr; GpHatch* brush=nullptr;
            if(GdipCreateBitmapFromScan0(8,8,32,PixelFormat32bppARGB,data.data(),&bitmap)!=Ok) return 2;
            if(GdipGetImageGraphicsContext(reinterpret_cast<GpImage*>(bitmap),&graphics)!=Ok) return 3;
            if(GdipSetCompositingMode(graphics,CompositingModeSourceCopy)!=Ok) return 4;
            if(GdipCreateHatchBrush(static_cast<HatchStyle>(style),fg[variant],bg[variant],&brush)!=Ok) return 5;
            if(GdipFillRectangleI(graphics,reinterpret_cast<GpBrush*>(brush),0,0,8,8)!=Ok) return 6;
            GdipDeleteGraphics(graphics);
            if(variant)out<<',';out<<'[';
            for(int i=0;i<64;i++){if(i)out<<',';out<<'['<<int(data[i*4+2])<<','<<int(data[i*4+1])<<','<<int(data[i*4])<<','<<int(data[i*4+3])<<']';}
            out<<']';GdipDeleteBrush(reinterpret_cast<GpBrush*>(brush));GdipDisposeImage(reinterpret_cast<GpImage*>(bitmap));
        }
        out<<"]}";
    }
    out<<"]\n";out.close();shutdown(token);std::puts("Recorded all 53 native 8x8 hatch tiles");return 0;
}
