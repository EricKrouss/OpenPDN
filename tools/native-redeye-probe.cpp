// Developer oracle: execute Microsoft's separately obtained GDI+ implementation.
// The Python driver checks the exact DLL hash before enabling analysis tracing.
#include <windows.h>
#include <gdiplus.h>
#include <cstdio>
#include <fstream>
#include <vector>
using namespace Gdiplus;

static unsigned char* breakpoint = nullptr;
static unsigned char saved_instruction;
static std::ofstream analysis;
static LONG CALLBACK capture_analysis(EXCEPTION_POINTERS* exception) {
    if (exception->ExceptionRecord->ExceptionAddress != breakpoint)
        return EXCEPTION_CONTINUE_SEARCH;
    // CRedEyeCorrection::HrFixRedEyeArea, version 10.0.19041.329 only.
    analysis.write(reinterpret_cast<char*>(exception->ContextRecord->Rcx), 0x458);
    const auto mxcsr = exception->ContextRecord->MxCsr;
    analysis.write(reinterpret_cast<const char*>(&mxcsr), sizeof(mxcsr));
    *breakpoint = saved_instruction;
    exception->ContextRecord->Rip = reinterpret_cast<uintptr_t>(breakpoint);
    return EXCEPTION_CONTINUE_EXECUTION;
}

static void pixels(std::ostream& out, const std::vector<unsigned char>& data) {
    out << '[';
    for (size_t i = 0; i < data.size(); i += 4) {
        if (i) out << ',';
        out << '[' << int(data[i+2]) << ',' << int(data[i+1]) << ','
            << int(data[i]) << ',' << int(data[i+3]) << ']';
    }
    out << ']';
}
int main(int argc, char** argv) {
    if (argc != 3 && argc != 4) {
        std::fprintf(stderr, "Usage: native-redeye-probe DLL OUTPUT.json [ANALYSIS.bin]\n");
        return 1;
    }
    char path[MAX_PATH];
    if (!GetFullPathNameA(argv[1], MAX_PATH, path, nullptr)) return 1;
    auto dll = LoadLibraryExA(path, nullptr, LOAD_WITH_ALTERED_SEARCH_PATH);
    if (!dll) { std::fprintf(stderr, "LoadLibrary: %lu\n", GetLastError()); return 1; }
#define API(name) auto name = reinterpret_cast<decltype(&Gdiplus::DllExports::name)>(GetProcAddress(dll, #name)); if (!name) return 1
    API(GdipCreateBitmapFromScan0); API(GdipCreateEffect); API(GdipSetEffectParameters);
    API(GdipBitmapApplyEffect); API(GdipDisposeImage); API(GdipDeleteEffect);
    auto startup = reinterpret_cast<decltype(&GdiplusStartup)>(GetProcAddress(dll, "GdiplusStartup"));
    auto shutdown = reinterpret_cast<decltype(&GdiplusShutdown)>(GetProcAddress(dll, "GdiplusShutdown"));
    if (!startup || !shutdown) return 1;
    ULONG_PTR token; GdiplusStartupInput input;
    if (startup(&token, &input, nullptr) != Ok) return 1;
    if (argc == 4) {
        analysis.open(argv[3], std::ios::binary);
        if (!analysis) return 1;
        breakpoint = reinterpret_cast<unsigned char*>(dll) + 0x142630;
        DWORD old;
        if (!VirtualProtect(breakpoint, 1, PAGE_EXECUTE_READWRITE, &old)) return 1;
        saved_instruction = *breakpoint;
        AddVectoredExceptionHandler(1, capture_analysis);
    }
    const GUID guid = {0x74d29d05,0x69a4,0x4266,{0x95,0x49,0x3c,0xc5,0x28,0x36,0xb6,0x32}};
    std::ofstream out(argv[2]);
    if (!out) return 1;
    out << '['; bool first = true;
    for (int background : {0,1}) for (int radius : {0,2,3,5,8}) {
        constexpr int width=41, height=33;
        std::vector<unsigned char> data(width*height*4);
        for (int y=0; y<height; ++y) for (int x=0; x<width; ++x) {
            const int i=(y*width+x)*4;
            const bool red=(x-20)*(x-20)+(y-16)*(y-16)<=radius*radius;
            data[i]=red?26:(background?255:120);
            data[i+1]=red?18:(background?255:140);
            data[i+2]=red?245:(background?255:170); data[i+3]=255;
        }
        const auto original=data;
        GpBitmap* bitmap=nullptr; CGpEffect* effect=nullptr;
        if (GdipCreateBitmapFromScan0(width,height,width*4,PixelFormat32bppARGB,data.data(),&bitmap)!=Ok) return 2;
        if (GdipCreateEffect(guid,&effect)!=Ok) return 3;
        RECT area{0,0,width,height}; RedEyeCorrectionParams params{1,&area};
        if (GdipSetEffectParameters(effect,&params,sizeof(params)+sizeof(RECT))!=Ok) return 4;
        for (int strength=1; strength<=5; ++strength) {
            if (breakpoint) *breakpoint=0xcc;
            if (GdipBitmapApplyEffect(bitmap,effect,&area,FALSE,nullptr,nullptr)!=Ok) return 5;
            if (!first) out << ','; first=false;
            out << "{\"width\":" << width << ",\"height\":" << height
                << ",\"radius\":" << radius << ",\"background\":" << background
                << ",\"strength\":" << strength << ",\"input\":";
            pixels(out,original); out << ",\"output\":"; pixels(out,data); out << '}';
        }
        GdipDeleteEffect(effect); GdipDisposeImage(reinterpret_cast<GpImage*>(bitmap));
    }
    out << "]\n"; out.close(); analysis.close(); shutdown(token);
    std::puts("Recorded 50 native Red Eye cases"); return 0;
}
